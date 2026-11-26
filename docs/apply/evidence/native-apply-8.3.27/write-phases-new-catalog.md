# Write phases

38728 writes/DDL statements in 74 blocks. A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).

| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |
|---|---|---|---|---|---|---|---|---|
| 1 | +00:12.658 .. +00:12.689 | 175 | Params | 2 | 1 | begin 1, commit 1 | DELETE <guid>.ui x1; UPDATE <guid>.ui x1 | `54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui` |
| 2 | +00:15.256 .. +00:15.486 | 175 | ConfigSave | 1 | 0 |  | DELETE LIKE %.new x1 | `LIKE %.new` |
| 3 | +00:15.488 .. +01:08.474 | 175 | Config | 19231 | 9632 |  | DELETE LIKE %.new x1; DELETE <guid>.new <- ConfigSave x4930; INSERT <guid>.new <- ConfigSave x4930; DELETE <guid>.<n>.new <- ConfigSave x4665; INSERT <guid>.<n>.new <- ConfigSave x4665; DELETE <guid>.1c.new <- ConfigSave x14; INSERT <guid>.1c.new <- ConfigSave x14; DELETE deleted.new <- ConfigSave x1; ... +11 more | `LIKE %.new` .. `versions.new` |
| 4 | +01:08.515 .. +01:08.556 | 175 | Files | 3 | 2 | begin 1, commit 1 | INSERT MobileVersions.datNEW x1; DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 5 | +01:08.664 .. +01:08.817 | 175 | Params | 16 | 8 | begin 4, commit 4 | INSERT DBNames.New x1; DELETE DBNames.New x2; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x2; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x4; ... +1 more | `DBNames.New` .. `DBNames-Ext-b3fa0ef0-9968-11f1-8f4f-00e04c68009...` |
| 6 | +01:11.487 .. +01:11.501 | 86 | _ExtensionsRestructNGS | 1 | 3 |  | DELETE x1 |  |
| 7 | +01:14.119 .. +01:14.149 | 86 | _Reference11035NG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 8 | +01:14.151 .. +01:14.154 | 86 | _DbCopiesInfoBaseUseNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 9 | +01:14.157 .. +01:14.162 | 86 | _DbCopiesUpdateTableStatNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 10 | +01:14.166 .. +01:14.168 | 86 | _DbCopiesUpdateStatNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 11 | +01:14.169 .. +01:14.177 | 86 | _WebSocketClientsNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 12 | +01:14.396 .. +01:14.438 | 86 | _Reference11035_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 13 | +01:14.451 .. +01:14.454 | 86 | _Reference11035_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 14 | +01:14.470 .. +01:14.472 | 86 | _Reference11035_3NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 15 | +01:14.481 .. +01:14.497 | 86 | _Reference11035_S_HPKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 16 | +01:14.505 .. +01:14.507 | 86 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 17 | +01:15.591 .. +01:15.598 | 86 | _Reference12NG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 18 | +01:15.600 .. +01:15.601 | 86 | _Reference12_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 19 | +01:15.602 .. +01:15.611 | 86 | _Reference12_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 20 | +01:15.627 .. +01:15.632 | 86 | _Reference12_3NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 21 | +01:15.641 .. +01:15.693 | 86 | _Reference12_S_HPKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 22 | +01:15.704 .. +01:15.709 | 86 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 23 | +01:15.735 .. +01:15.736 | 86 | _Reference12_1NG | 1 | 0 |  | DROP INDEX x1 |  |
| 24 | +01:15.742 .. +01:15.744 | 86 | _Reference12_2NG | 1 | 0 |  | DROP INDEX x1 |  |
| 25 | +01:15.750 .. +01:15.752 | 86 | _Reference12_3NG | 1 | 0 |  | DROP INDEX x1 |  |
| 26 | +01:15.756 .. +01:15.758 | 86 | _Reference12_S_HPKNG | 1 | 0 |  | DROP INDEX x1 |  |
| 27 | +01:15.766 .. +01:15.782 | 86 | _Reference12NG | 1 | 9 |  | INSERT x1 |  |
| 28 | +01:15.792 .. +01:15.811 | 86 | _Reference12_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 29 | +01:15.821 .. +01:15.832 | 86 | _Reference12_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 30 | +01:15.847 .. +01:15.864 | 86 | _Reference12_3NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 31 | +01:15.864 .. +01:15.901 | 86 | _Reference12_S_HPKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 32 | +01:21.534 .. +01:21.551 | 86 | _ConfigChngRNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 33 | +01:21.555 .. +01:21.565 | 86 | _ConfigChngR_ExtPropsNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 34 | +01:21.574 .. +01:21.577 | 86 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 35 | +01:21.585 .. +01:21.590 | 86 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 36 | +01:21.591 .. +01:21.600 | 86 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 37 | +01:21.603 .. +01:21.605 | 86 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 38 | +01:21.621 .. +01:21.630 | 86 | _ConfigChngR_1NG | 1 | 0 |  | DROP INDEX x1 |  |
| 39 | +01:21.634 .. +01:21.635 | 86 | _ConfigChngR_2NG | 1 | 0 |  | DROP INDEX x1 |  |
| 40 | +01:21.647 .. +01:21.649 | 86 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | DROP INDEX x1 |  |
| 41 | +01:22.246 .. +01:22.281 | 86 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 42 | +01:22.285 .. +01:22.317 | 86 | _ConfigChngR_ExtPropsNG | 1 | 9940 |  | INSERT BULK x1 |  |
| 43 | +01:22.900 .. +01:22.980 | 86 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 44 | +01:22.989 .. +01:23.060 | 86 | _ConfigChngR_ExtPropsNG | 2 | 10610 |  | INSERT BULK x2 |  |
| 45 | +01:23.348 .. +01:23.359 | 86 | _ConfigChngRNG | 1 | 688 |  | INSERT BULK x1 |  |
| 46 | +01:23.368 .. +01:23.399 | 86 | _ConfigChngR_ExtPropsNG | 1 | 807 |  | INSERT BULK x1 |  |
| 47 | +01:23.402 .. +01:23.478 | 86 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 48 | +01:23.491 .. +01:23.555 | 86 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 49 | +01:23.558 .. +01:23.609 | 86 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 50 | +01:23.627 .. +01:23.702 | 86 | _ExtensionsRestruct | 4 | 4 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 51 | +01:26.000 .. +01:26.043 | 86 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 52 | +01:31.918 .. +01:32.018 | 86 | _ExtensionsRestructNGS | 4 | 2 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 53 | +01:34.515 .. +01:34.527 | 86 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 54 | +01:34.640 .. +01:34.759 | 86 | _ExtensionsRestructNGS | 4 | 3 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 55 | +01:34.879 .. +01:34.896 | 86 | Files | 4 | 2 | begin 1, commit 1 | DELETE extd_props_cached/gc.mrk x2; INSERT extd_props_cached/gc.mrk x1; UPDATE extd_props_cached/gc.mrk x1 | `extd_props_cached/gc.mrk` |
| 56 | +01:34.906 .. +01:34.990 | 86 | Params | 12 | 8 | begin 4, commit 4 | INSERT DBNames.New x1; DELETE DBNames.New x1; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x1; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x2; ... +1 more | `DBNames.New` .. `DBNames-Ext-b3fa0ef0-9968-11f1-8f4f-00e04c68009...` |
| 57 | +01:40.126 .. +01:43.032 | 86 | Params | 48 | 32 | begin 16, commit 16 | INSERT <guid>.sinew x16; DELETE <guid>.sinew x16; UPDATE <guid>.sinew x16 | `42ed49cc-765d-4314-bc2d-af425af7bf13.sinew` .. `e05c0074-0404-4b7a-835e-9cacd405960e.sinew` |
| 58 | +01:43.334 .. +01:43.362 | 86 | Config | 3 | 2 | begin 1, commit 1 | INSERT commit x1; DELETE commit x1; UPDATE commit x1 | `commit` |
| 59 | +01:43.381 .. +01:43.387 | 86 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 60 | +01:43.412 .. +01:43.467 | 86 | _Reference12 | 1 | 0 |  | DROP TABLE x1 |  |
| 61 | +01:43.483 .. +01:43.514 | 86 | _ConfigChngR | 1 | 0 |  | DROP TABLE x1 |  |
| 62 | +01:43.529 .. +01:43.550 | 86 | _ConfigChngR_ExtProps | 1 | 0 |  | DROP TABLE x1 |  |
| 63 | +01:43.552 .. +01:43.562 | 86 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 64 | +01:43.609 .. +01:44.405 | 86 |  | 19 | 0 | begin 19, commit 19 | RENAME exec sp_rename '_Reference11035NG', '_Reference11035', 'object' x1; RENAME exec sp_rename '_Reference11035._Reference11035_1NG', '_Reference11035_1', 'index' x1; RENAME exec sp_rename '_Reference11035._Reference11035_2NG', '_Reference11035_2', 'index' x1; RENAME exec sp_rename '_Reference11035._Reference11035_3NG', '_Reference11035_3', 'index' x1; RENAME exec sp_rename '_Reference11035._Reference11035_S_HPKNG', '_Reference11035_S_HPK', 'index' x1; RENAME exec sp_rename '_DbCopiesInfoBaseUseNG', '_DbCopiesInfoBaseUse', 'object' x1; RENAME exec sp_rename '_DbCopiesUpdateTableStatNG', '_DbCopiesUpdateTableStat', 'object' x1; RENAME exec sp_rename '_DbCopiesUpdateStatNG', '_DbCopiesUpdateStat', 'object' x1; ... +11 more |  |
| 65 | +01:44.891 .. +01:44.894 | 86 | PK___ConfigC__AC8ED0C4ABCBAD3C | 1 | 0 |  | ALTER INDEX x1 |  |
| 66 | +01:44.969 .. +01:45.021 | 86 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 67 | +01:45.099 .. +01:45.167 | 86 | DBSchema | 1 | 1 |  | UPDATE x1 |  |
| 68 | +01:45.448 .. +01:47.057 | 86 | Params | 42 | 36 | begin 2, commit 2 | DELETE DBNames <- Params x1; UPDATE DBNames.New -> DBNames x1; DELETE DBNamesVersion-DBNames x1; UPDATE DBNamesVersion-DBNames x1; DELETE DBNames-Ext-1.New x1; DELETE DBNames-Ext-<guid>.New x2; DELETE <guid>.si <- Params x16; UPDATE <guid>.sinew -> <guid>.si x16; ... +3 more | `DBNames` .. `DynamicallyUpdated` |
| 69 | +01:47.073 .. +02:31.750 | 86 | Config | 19230 | 19249 |  | DELETE deleted.new x1; DELETE <guid> <- Config x4930; UPDATE <guid>.new -> <guid> x4930; DELETE <guid>.<n> <- Config x4665; UPDATE <guid>.<n>.new -> <guid>.<n> x4665; DELETE <guid>.1c <- Config x14; UPDATE <guid>.1c.new -> <guid>.1c x14; DELETE <guid>.a <- Config x1; ... +10 more | `deleted.new` .. `DynamicallyUpdated` |
| 70 | +02:31.757 .. +02:35.947 | 86 | ConfigSave | 1 | 0 |  | DELETE LIKE % x1 | `LIKE %` |
| 71 | +02:35.953 .. +02:35.971 | 86 | Files | 2 | 2 |  | DELETE MobileVersions.dat <- Files x1; UPDATE MobileVersions.datNEW -> MobileVersions.dat x1 | `MobileVersions.dat` .. `MobileVersions.datNEW` |
| 72 | +02:35.976 .. +02:35.982 | 86 | Config | 3 | 0 |  | DELETE commit x1; DELETE dynamicCommit x1; DELETE dbStruFinal x1 | `commit` .. `dbStruFinal` |
| 73 | +02:36.033 .. +02:36.037 | 86 | Params | 4 | 2 | begin 2, commit 2 | DELETE <guid>.ui x2; UPDATE <guid>.ui x2 | `789702c6-d272-4008-9e55-db6f10687ae0.ui` .. `97f2c291-1aa3-4d96-8266-82a958c7dfaf.ui` |
| 74 | +02:36.221 .. +02:36.273 | 86 | Files | 30 | 18 | begin 3, commit 3 | DELETE userDocs_ru.new x2; INSERT userDocs_ru.new x1; UPDATE userDocs_ru.new x4; DELETE userVocabulary_ru.new x2; INSERT userVocabulary_ru.new x1; UPDATE userVocabulary_ru.new x4; DELETE userPostings_ru.new x2; INSERT userPostings_ru.new x1; ... +10 more | `userDocs_ru.new` .. `userPostings_ru.new` |
