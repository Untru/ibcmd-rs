# Write phases

12777 writes/DDL statements in 83 blocks. A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).

| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |
|---|---|---|---|---|---|---|---|---|
| 1 | +00:07.095 .. +00:07.096 | 198 | Params | 2 | 1 | begin 1, commit 1 | DELETE <guid>.ui x1; UPDATE <guid>.ui x1 | `54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui` |
| 2 | +00:07.211 .. +00:07.215 | 198 | ConfigSave | 1 | 0 |  | DELETE LIKE %.new x1 | `LIKE %.new` |
| 3 | +00:07.221 .. +00:07.307 | 198 | Config | 19 | 9 |  | DELETE LIKE %.new x1; DELETE <guid>.new <- ConfigSave x2; INSERT <guid>.new <- ConfigSave x2; DELETE <guid>.<n>.new <- ConfigSave x4; INSERT <guid>.<n>.new <- ConfigSave x4; DELETE root.new <- ConfigSave x1; INSERT root.new <- ConfigSave x1; DELETE version.new <- ConfigSave x1; ... +3 more | `LIKE %.new` .. `versions.new` |
| 4 | +00:07.314 .. +00:07.319 | 198 | Files | 3 | 2 | begin 1, commit 1 | INSERT MobileVersions.datNEW x1; DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 5 | +00:07.368 .. +00:07.526 | 198 | Params | 16 | 8 | begin 4, commit 4 | INSERT DBNames.New x1; DELETE DBNames.New x2; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x2; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x4; ... +1 more | `DBNames.New` .. `DBNames-Ext-b3fa0ef0-9968-11f1-8f4f-00e04c68009...` |
| 6 | +00:08.236 .. +00:08.237 | 153 | _ExtensionsRestructNGS | 1 | 0 |  | DELETE x1 |  |
| 7 | +00:08.262 .. +00:08.276 | 153 | _Reference12NG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 8 | +00:08.340 .. +00:08.343 | 153 | _Reference12_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 9 | +00:08.347 .. +00:08.351 | 153 | _Reference12_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 10 | +00:08.353 .. +00:08.354 | 153 | _Reference12_3NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 11 | +00:08.354 .. +00:08.360 | 153 | _Reference12_S_HPKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 12 | +00:08.365 .. +00:08.367 | 153 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 13 | +00:08.372 .. +00:08.373 | 153 | _Reference12_1NG | 1 | 0 |  | DROP INDEX x1 |  |
| 14 | +00:08.374 .. +00:08.375 | 153 | _Reference12_2NG | 1 | 0 |  | DROP INDEX x1 |  |
| 15 | +00:08.376 .. +00:08.377 | 153 | _Reference12_3NG | 1 | 0 |  | DROP INDEX x1 |  |
| 16 | +00:08.381 .. +00:08.383 | 153 | _Reference12_S_HPKNG | 1 | 0 |  | DROP INDEX x1 |  |
| 17 | +00:08.385 .. +00:08.390 | 153 | _Reference12NG | 1 | 9 |  | INSERT x1 |  |
| 18 | +00:08.392 .. +00:08.405 | 153 | _Reference12_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 19 | +00:08.406 .. +00:08.408 | 153 | _Reference12_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 20 | +00:08.409 .. +00:08.411 | 153 | _Reference12_3NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 21 | +00:08.412 .. +00:08.421 | 153 | _Reference12_S_HPKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 22 | +00:09.038 .. +00:09.043 | 153 | _DbCopiesUpdatesNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 23 | +00:09.043 .. +00:09.045 | 153 | _DbCopiesUpdates_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 24 | +00:09.046 .. +00:09.046 | 153 | SchemaStorage | 1 | 1 | begin 1 | UPDATE x1 |  |
| 25 | +00:09.048 .. +00:09.049 | 153 | _DbCopiesUpdatesNG | 1 | 0 | commit 1 | INSERT x1 |  |
| 26 | +00:09.052 .. +00:09.057 | 153 | _DbCopiesNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 27 | +00:09.057 .. +00:09.059 | 153 | _DbCopies_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 28 | +00:09.060 .. +00:09.060 | 153 | SchemaStorage | 1 | 1 | begin 1 | UPDATE x1 |  |
| 29 | +00:09.062 .. +00:09.063 | 153 | _DbCopiesNG | 1 | 0 | commit 1 | INSERT x1 |  |
| 30 | +00:09.295 .. +00:09.298 | 153 | _ConfigChngRNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 31 | +00:09.299 .. +00:09.301 | 153 | _ConfigChngR_ExtPropsNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 32 | +00:09.302 .. +00:09.303 | 153 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 33 | +00:09.304 .. +00:09.305 | 153 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 34 | +00:09.306 .. +00:09.308 | 153 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 35 | +00:09.309 .. +00:09.309 | 153 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 36 | +00:09.312 .. +00:09.313 | 153 | _ConfigChngR_1NG | 1 | 0 |  | DROP INDEX x1 |  |
| 37 | +00:09.315 .. +00:09.316 | 153 | _ConfigChngR_2NG | 1 | 0 |  | DROP INDEX x1 |  |
| 38 | +00:09.317 .. +00:09.318 | 153 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | DROP INDEX x1 |  |
| 39 | +00:09.432 .. +00:09.447 | 153 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 40 | +00:09.448 .. +00:09.456 | 153 | _ConfigChngR_ExtPropsNG | 1 | 9996 |  | INSERT BULK x1 |  |
| 41 | +00:09.553 .. +00:09.575 | 153 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 42 | +00:09.576 .. +00:09.590 | 153 | _ConfigChngR_ExtPropsNG | 2 | 10658 |  | INSERT BULK x2 |  |
| 43 | +00:09.648 .. +00:09.653 | 153 | _ConfigChngRNG | 1 | 685 |  | INSERT BULK x1 |  |
| 44 | +00:09.655 .. +00:09.657 | 153 | _ConfigChngR_ExtPropsNG | 1 | 703 |  | INSERT BULK x1 |  |
| 45 | +00:09.658 .. +00:09.677 | 153 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 46 | +00:09.678 .. +00:09.695 | 153 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 47 | +00:09.697 .. +00:09.722 | 153 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 48 | +00:09.727 .. +00:09.735 | 153 | _ExtensionsRestruct | 4 | 4 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 49 | +00:10.192 .. +00:10.193 | 153 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 50 | +00:10.370 .. +00:10.409 | 153 | _ExtensionsRestructNGS | 4 | 2 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 51 | +00:10.822 .. +00:10.823 | 153 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 52 | +00:10.859 .. +00:10.886 | 153 | _ExtensionsRestructNGS | 4 | 3 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 53 | +00:10.933 .. +00:10.937 | 153 | Files | 4 | 2 | begin 1, commit 1 | DELETE extd_props_cached/gc.mrk x2; INSERT extd_props_cached/gc.mrk x1; UPDATE extd_props_cached/gc.mrk x1 | `extd_props_cached/gc.mrk` |
| 54 | +00:10.983 .. +01:39.041 | 153 | ConfigCAS | 12161 | 0 |  | DELETE <sha1> x12160; DELETE dbStruFinal<guid> x1 | `0002268f0fd24ea1bbde19a2e10fdc82bdcd9203` .. `ffe8c03da86fce9b38d6757817de0f982248d70c` |
| 55 | +01:39.049 .. +01:39.996 | 153 | Files | 311 | 2 | begin 1, commit 1 | DELETE userDocs_ru_<sha1>.bin x103; DELETE userPostings_ru_<sha1>.bin x103; DELETE userVocabulary_ru_<sha1>.bin x103; DELETE CAS_GC_Info x1; UPDATE CAS_GC_Info x1 | `userDocs_ru_0013adb20fcd7ec5b74a122401acf41c7a6...` .. `CAS_GC_Info` |
| 56 | +01:40.019 .. +01:46.867 | 153 | Params | 60 | 40 | begin 20, commit 20 | INSERT DBNames.New x1; DELETE DBNames.New x1; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x1; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x2; ... +4 more | `DBNames.New` .. `e05c0074-0404-4b7a-835e-9cacd405960e.sinew` |
| 57 | +01:46.999 .. +01:47.012 | 153 | Config | 3 | 2 | begin 1, commit 1 | INSERT commit x1; DELETE commit x1; UPDATE commit x1 | `commit` |
| 58 | +01:47.019 .. +01:47.023 | 153 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 59 | +01:47.029 .. +01:47.039 | 153 | _Reference12 | 1 | 0 |  | DROP TABLE x1 |  |
| 60 | +01:47.043 .. +01:47.045 | 153 | _DbCopiesUpdates | 1 | 0 |  | DROP TABLE x1 |  |
| 61 | +01:47.050 .. +01:47.052 | 153 | _DbCopies | 1 | 0 |  | DROP TABLE x1 |  |
| 62 | +01:47.055 .. +01:47.061 | 153 | _ConfigChngR | 1 | 0 |  | DROP TABLE x1 |  |
| 63 | +01:47.068 .. +01:47.070 | 153 | _ConfigChngR_ExtProps | 1 | 0 |  | DROP TABLE x1 |  |
| 64 | +01:47.070 .. +01:47.071 | 153 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 65 | +01:47.078 .. +01:47.643 | 153 |  | 14 | 0 | begin 14, commit 14 | RENAME exec sp_rename '_Reference12NG', '_Reference12', 'object' x1; RENAME exec sp_rename '_Reference12._Reference12_1NG', '_Reference12_1', 'index' x1; RENAME exec sp_rename '_Reference12._Reference12_2NG', '_Reference12_2', 'index' x1; RENAME exec sp_rename '_Reference12._Reference12_3NG', '_Reference12_3', 'index' x1; RENAME exec sp_rename '_Reference12._Reference12_S_HPKNG', '_Reference12_S_HPK', 'index' x1; RENAME exec sp_rename '_DbCopiesUpdatesNG', '_DbCopiesUpdates', 'object' x1; RENAME exec sp_rename '_DbCopiesUpdates._DbCopiesUpdates_1NG', '_DbCopiesUpdates_1', 'index' x1; RENAME exec sp_rename '_DbCopiesNG', '_DbCopies', 'object' x1; ... +6 more |  |
| 66 | +01:47.776 .. +01:47.779 | 153 | PK___Referen__AC8ED0C4077A8F3B | 1 | 0 |  | ALTER INDEX x1 |  |
| 67 | +01:47.779 .. +01:47.781 | 153 | PK___Referen__AC8ED0C4DBBB6986 | 1 | 0 |  | ALTER INDEX x1 |  |
| 68 | +01:47.782 .. +01:47.783 | 153 | PK___Referen__AC8ED0C4720747AF | 1 | 0 |  | ALTER INDEX x1 |  |
| 69 | +01:47.784 .. +01:47.785 | 153 | PK___Enum108__AC8ED0C45E875D60 | 1 | 0 |  | ALTER INDEX x1 |  |
| 70 | +01:47.785 .. +01:47.786 | 153 | PK___Enum110__AC8ED0C4A5E2EE79 | 1 | 0 |  | ALTER INDEX x1 |  |
| 71 | +01:47.787 .. +01:47.790 | 153 | PK___Documen__AC8ED0C48EB57390 | 1 | 0 |  | ALTER INDEX x1 |  |
| 72 | +01:47.790 .. +01:47.794 | 153 | PK___Referen__AC8ED0C43F77AD82 | 1 | 0 |  | ALTER INDEX x1 |  |
| 73 | +01:47.795 .. +01:47.796 | 153 | PK___Referen__AC8ED0C45FCE1C7A | 1 | 0 |  | ALTER INDEX x1 |  |
| 74 | +01:47.796 .. +01:47.796 | 153 | PK___ConfigC__AC8ED0C4C01EB0C9 | 1 | 0 |  | ALTER INDEX x1 |  |
| 75 | +01:47.817 .. +01:47.841 | 153 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 76 | +01:47.873 .. +01:47.898 | 153 | DBSchema | 1 | 1 |  | UPDATE x1 |  |
| 77 | +01:47.952 .. +01:48.229 | 153 | Params | 42 | 36 | begin 2, commit 2 | DELETE DBNames <- Params x1; UPDATE DBNames.New -> DBNames x1; DELETE DBNamesVersion-DBNames x1; UPDATE DBNamesVersion-DBNames x1; DELETE DBNames-Ext-1.New x1; DELETE DBNames-Ext-<guid>.New x2; DELETE <guid>.si <- Params x16; UPDATE <guid>.sinew -> <guid>.si x16; ... +3 more | `DBNames` .. `DynamicallyUpdated` |
| 78 | +01:48.261 .. +01:48.289 | 153 | Config | 19 | 18 |  | DELETE <guid>.<n> <- Config x4; UPDATE <guid>.<n>.new -> <guid>.<n> x4; DELETE <guid> <- Config x2; UPDATE <guid>.new -> <guid> x2; DELETE root <- Config x1; UPDATE root.new -> root x1; DELETE version <- Config x1; UPDATE version.new -> version x1; ... +3 more | `a28d7e32-fa63-4656-83b3-e0b001c7d122.0` .. `DynamicallyUpdated` |
| 79 | +01:48.289 .. +01:48.307 | 153 | ConfigSave | 1 | 0 |  | DELETE LIKE % x1 | `LIKE %` |
| 80 | +01:48.309 .. +01:48.320 | 153 | Files | 2 | 2 |  | DELETE MobileVersions.dat <- Files x1; UPDATE MobileVersions.datNEW -> MobileVersions.dat x1 | `MobileVersions.dat` .. `MobileVersions.datNEW` |
| 81 | +01:48.321 .. +01:48.326 | 153 | Config | 3 | 0 |  | DELETE commit x1; DELETE dynamicCommit x1; DELETE dbStruFinal x1 | `commit` .. `dbStruFinal` |
| 82 | +01:48.353 .. +01:48.357 | 153 | Params | 4 | 2 | begin 2, commit 2 | DELETE <guid>.ui x2; UPDATE <guid>.ui x2 | `789702c6-d272-4008-9e55-db6f10687ae0.ui` .. `97f2c291-1aa3-4d96-8266-82a958c7dfaf.ui` |
| 83 | +01:48.751 .. +01:49.022 | 153 | Files | 30 | 18 | begin 3, commit 3 | DELETE userDocs_ru.new x2; INSERT userDocs_ru.new x1; UPDATE userDocs_ru.new x4; DELETE userVocabulary_ru.new x2; INSERT userVocabulary_ru.new x1; UPDATE userVocabulary_ru.new x4; DELETE userPostings_ru.new x2; INSERT userPostings_ru.new x1; ... +10 more | `userDocs_ru.new` .. `userPostings_ru.new` |
