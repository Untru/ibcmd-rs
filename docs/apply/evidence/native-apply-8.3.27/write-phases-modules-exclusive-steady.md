# Write phases

372 writes/DDL statements in 47 blocks. A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).

| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |
|---|---|---|---|---|---|---|---|---|
| 1 | +00:09.546 .. +00:09.571 | 174 | Params | 2 | 1 | begin 1, commit 1 | DELETE <guid>.ui x1; UPDATE <guid>.ui x1 | `54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui` |
| 2 | +00:09.782 .. +00:09.785 | 174 | ConfigSave | 1 | 0 |  | DELETE LIKE %.new x1 | `LIKE %.new` |
| 3 | +00:09.791 .. +00:10.493 | 174 | Config | 77 | 38 |  | DELETE LIKE %.new x1; DELETE <guid>.new <- ConfigSave x14; INSERT <guid>.new <- ConfigSave x14; DELETE <guid>.<n>.new <- ConfigSave x21; INSERT <guid>.<n>.new <- ConfigSave x21; DELETE root.new <- ConfigSave x1; INSERT root.new <- ConfigSave x1; DELETE version.new <- ConfigSave x1; ... +3 more | `LIKE %.new` .. `versions.new` |
| 4 | +00:10.506 .. +00:10.556 | 174 | Files | 3 | 2 | begin 1, commit 1 | INSERT MobileVersions.datNEW x1; DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 5 | +00:10.672 .. +00:10.807 | 174 | Params | 16 | 8 | begin 4, commit 4 | INSERT DBNames.New x1; DELETE DBNames.New x2; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x2; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x4; ... +1 more | `DBNames.New` .. `DBNames-Ext-b3fa0ef0-9968-11f1-8f4f-00e04c68009...` |
| 6 | +00:14.100 .. +00:14.120 | 140 | _ExtensionsRestructNGS | 1 | 3 |  | DELETE x1 |  |
| 7 | +00:14.197 .. +00:14.302 | 140 | _ConfigChngRNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 8 | +00:14.305 .. +00:14.336 | 140 | _ConfigChngR_ExtPropsNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 9 | +00:14.487 .. +00:14.494 | 140 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 10 | +00:14.498 .. +00:14.503 | 140 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 11 | +00:14.517 .. +00:14.550 | 140 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 12 | +00:14.597 .. +00:14.619 | 140 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 13 | +00:14.632 .. +00:14.644 | 140 | _ConfigChngR_1NG | 1 | 0 |  | DROP INDEX x1 |  |
| 14 | +00:14.649 .. +00:14.651 | 140 | _ConfigChngR_2NG | 1 | 0 |  | DROP INDEX x1 |  |
| 15 | +00:14.658 .. +00:14.669 | 140 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | DROP INDEX x1 |  |
| 16 | +00:15.126 .. +00:15.172 | 140 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 17 | +00:15.177 .. +00:15.215 | 140 | _ConfigChngR_ExtPropsNG | 1 | 9938 |  | INSERT BULK x1 |  |
| 18 | +00:15.762 .. +00:15.820 | 140 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 19 | +00:15.826 .. +00:15.872 | 140 | _ConfigChngR_ExtPropsNG | 2 | 10617 |  | INSERT BULK x2 |  |
| 20 | +00:15.956 .. +00:15.961 | 140 | _ConfigChngRNG | 1 | 685 |  | INSERT BULK x1 |  |
| 21 | +00:15.962 .. +00:15.966 | 140 | _ConfigChngR_ExtPropsNG | 1 | 802 |  | INSERT BULK x1 |  |
| 22 | +00:15.968 .. +00:15.998 | 140 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 23 | +00:16.000 .. +00:16.028 | 140 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 24 | +00:16.029 .. +00:16.065 | 140 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 25 | +00:16.072 .. +00:16.088 | 140 | _ExtensionsRestruct | 4 | 4 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 26 | +00:16.678 .. +00:16.680 | 140 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 27 | +00:16.903 .. +00:16.943 | 140 | _ExtensionsRestructNGS | 4 | 2 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 28 | +00:18.262 .. +00:18.264 | 140 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 29 | +00:18.331 .. +00:18.384 | 140 | _ExtensionsRestructNGS | 4 | 3 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 30 | +00:18.457 .. +00:18.468 | 140 | Files | 4 | 2 | begin 1, commit 1 | DELETE extd_props_cached/gc.mrk x2; INSERT extd_props_cached/gc.mrk x1; UPDATE extd_props_cached/gc.mrk x1 | `extd_props_cached/gc.mrk` |
| 31 | +00:18.471 .. +00:25.117 | 140 | Params | 60 | 40 | begin 20, commit 20 | INSERT DBNames.New x1; DELETE DBNames.New x1; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x1; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x2; ... +4 more | `DBNames.New` .. `e05c0074-0404-4b7a-835e-9cacd405960e.sinew` |
| 32 | +00:25.279 .. +00:25.296 | 140 | Config | 3 | 2 | begin 1, commit 1 | INSERT commit x1; DELETE commit x1; UPDATE commit x1 | `commit` |
| 33 | +00:25.305 .. +00:25.308 | 140 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 34 | +00:25.315 .. +00:25.376 | 140 | _ConfigChngR | 1 | 0 |  | DROP TABLE x1 |  |
| 35 | +00:25.378 .. +00:25.389 | 140 | _ConfigChngR_ExtProps | 1 | 0 |  | DROP TABLE x1 |  |
| 36 | +00:25.390 .. +00:25.390 | 140 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 37 | +00:25.397 .. +00:26.212 | 140 |  | 5 | 0 | begin 5, commit 5 | RENAME exec sp_rename '_ConfigChngRNG', '_ConfigChngR', 'object' x1; RENAME exec sp_rename '_ConfigChngR_ExtPropsNG', '_ConfigChngR_ExtProps', 'object' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_1NG', '_ConfigChngR_1', 'index' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_2NG', '_ConfigChngR_2', 'index' x1; RENAME exec sp_rename '_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG', '_ConfigChngR_ExtPro... x1 |  |
| 38 | +00:26.364 .. +00:26.367 | 140 | PK___ConfigC__AC8ED0C424724256 | 1 | 0 |  | ALTER INDEX x1 |  |
| 39 | +00:26.400 .. +00:26.422 | 140 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 40 | +00:26.470 .. +00:26.504 | 140 | DBSchema | 1 | 1 |  | UPDATE x1 |  |
| 41 | +00:26.582 .. +00:27.621 | 140 | Params | 39 | 33 | begin 1, commit 1 | DELETE DBNames.New x1; DELETE DBNames-Ext-1.New x1; DELETE DBNames-Ext-<guid>.New x2; DELETE <guid>.si <- Params x16; UPDATE <guid>.sinew -> <guid>.si x16; DELETE siVersions x1; UPDATE siVersions x1; DELETE DynamicallyUpdated x1 | `DBNames.New` .. `DynamicallyUpdated` |
| 42 | +00:27.660 .. +00:27.774 | 140 | Config | 77 | 76 |  | DELETE <guid>.<n> <- Config x21; UPDATE <guid>.<n>.new -> <guid>.<n> x21; DELETE <guid> <- Config x14; UPDATE <guid>.new -> <guid> x14; DELETE root <- Config x1; UPDATE root.new -> root x1; DELETE version <- Config x1; UPDATE version.new -> version x1; ... +3 more | `04332cbc-afdb-4c23-b188-b32fff0a1eb0.0` .. `DynamicallyUpdated` |
| 43 | +00:27.775 .. +00:27.782 | 140 | ConfigSave | 1 | 0 |  | DELETE LIKE % x1 | `LIKE %` |
| 44 | +00:27.786 .. +00:27.812 | 140 | Files | 2 | 2 |  | DELETE MobileVersions.dat <- Files x1; UPDATE MobileVersions.datNEW -> MobileVersions.dat x1 | `MobileVersions.dat` .. `MobileVersions.datNEW` |
| 45 | +00:27.814 .. +00:27.821 | 140 | Config | 3 | 0 |  | DELETE commit x1; DELETE dynamicCommit x1; DELETE dbStruFinal x1 | `commit` .. `dbStruFinal` |
| 46 | +00:27.858 .. +00:27.868 | 140 | Params | 4 | 2 | begin 2, commit 2 | DELETE <guid>.ui x2; UPDATE <guid>.ui x2 | `789702c6-d272-4008-9e55-db6f10687ae0.ui` .. `97f2c291-1aa3-4d96-8266-82a958c7dfaf.ui` |
| 47 | +00:28.325 .. +00:28.688 | 140 | Files | 30 | 18 | begin 3, commit 3 | DELETE userDocs_ru.new x2; INSERT userDocs_ru.new x1; UPDATE userDocs_ru.new x4; DELETE userVocabulary_ru.new x2; INSERT userVocabulary_ru.new x1; UPDATE userVocabulary_ru.new x4; DELETE userPostings_ru.new x2; INSERT userPostings_ru.new x1; ... +10 more | `userDocs_ru.new` .. `userPostings_ru.new` |
