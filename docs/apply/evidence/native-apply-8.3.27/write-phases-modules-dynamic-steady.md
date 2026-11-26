# Write phases

381 writes/DDL statements in 49 blocks. A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).

| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |
|---|---|---|---|---|---|---|---|---|
| 1 | +00:15.931 .. +00:15.975 | 179 | Params | 2 | 1 | begin 1, commit 1 | DELETE <guid>.ui x1; UPDATE <guid>.ui x1 | `54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui` |
| 2 | +00:16.225 .. +00:16.233 | 179 | ConfigSave | 1 | 0 |  | DELETE LIKE %.new x1 | `LIKE %.new` |
| 3 | +00:16.235 .. +00:17.224 | 179 | Config | 77 | 38 |  | DELETE LIKE %.new x1; DELETE <guid>.new <- ConfigSave x14; INSERT <guid>.new <- ConfigSave x14; DELETE <guid>.<n>.new <- ConfigSave x21; INSERT <guid>.<n>.new <- ConfigSave x21; DELETE root.new <- ConfigSave x1; INSERT root.new <- ConfigSave x1; DELETE version.new <- ConfigSave x1; ... +3 more | `LIKE %.new` .. `versions.new` |
| 4 | +00:17.250 .. +00:17.274 | 179 | Files | 3 | 2 | begin 1, commit 1 | INSERT MobileVersions.datNEW x1; DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 5 | +00:17.472 .. +00:17.771 | 179 | Params | 16 | 8 | begin 4, commit 4 | INSERT DBNames.New x1; DELETE DBNames.New x2; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x2; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x4; ... +1 more | `DBNames.New` .. `DBNames-Ext-b3fa0ef0-9968-11f1-8f4f-00e04c68009...` |
| 6 | +00:20.278 .. +00:20.301 | 55 | _ExtensionsRestructNGS | 1 | 3 |  | DELETE x1 |  |
| 7 | +00:20.374 .. +00:20.471 | 55 | _ConfigChngRNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 8 | +00:20.472 .. +00:20.497 | 55 | _ConfigChngR_ExtPropsNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 9 | +00:20.740 .. +00:20.754 | 55 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 10 | +00:20.754 .. +00:20.758 | 55 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 11 | +00:20.758 .. +00:20.791 | 55 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 12 | +00:20.797 .. +00:20.807 | 55 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 13 | +00:20.812 .. +00:20.815 | 55 | _ConfigChngR_1NG | 1 | 0 |  | DROP INDEX x1 |  |
| 14 | +00:20.815 .. +00:20.818 | 55 | _ConfigChngR_2NG | 1 | 0 |  | DROP INDEX x1 |  |
| 15 | +00:20.819 .. +00:20.821 | 55 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | DROP INDEX x1 |  |
| 16 | +00:20.977 .. +00:21.034 | 55 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 17 | +00:21.036 .. +00:21.075 | 55 | _ConfigChngR_ExtPropsNG | 1 | 9938 |  | INSERT BULK x1 |  |
| 18 | +00:21.774 .. +00:21.884 | 55 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 19 | +00:21.890 .. +00:22.035 | 55 | _ConfigChngR_ExtPropsNG | 2 | 10617 |  | INSERT BULK x2 |  |
| 20 | +00:22.131 .. +00:22.184 | 55 | _ConfigChngRNG | 1 | 685 |  | INSERT BULK x1 |  |
| 21 | +00:22.200 .. +00:22.266 | 55 | _ConfigChngR_ExtPropsNG | 1 | 802 |  | INSERT BULK x1 |  |
| 22 | +00:22.281 .. +00:22.373 | 55 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 23 | +00:22.373 .. +00:22.658 | 55 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 24 | +00:22.669 .. +00:22.799 | 55 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 25 | +00:22.811 .. +00:22.865 | 55 | _ExtensionsRestruct | 4 | 4 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 26 | +00:23.985 .. +00:23.988 | 55 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 27 | +00:24.181 .. +00:24.230 | 55 | _ExtensionsRestructNGS | 4 | 2 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 28 | +00:25.299 .. +00:25.301 | 55 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 29 | +00:25.361 .. +00:25.407 | 55 | _ExtensionsRestructNGS | 4 | 3 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 30 | +00:25.467 .. +00:25.475 | 55 | Files | 4 | 2 | begin 1, commit 1 | DELETE extd_props_cached/gc.mrk x2; INSERT extd_props_cached/gc.mrk x1; UPDATE extd_props_cached/gc.mrk x1 | `extd_props_cached/gc.mrk` |
| 31 | +00:25.478 .. +00:30.351 | 55 | Params | 60 | 40 | begin 20, commit 20 | INSERT DBNames.New x1; DELETE DBNames.New x1; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x1; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x2; ... +4 more | `DBNames.New` .. `e05c0074-0404-4b7a-835e-9cacd405960e.sinew` |
| 32 | +00:30.474 .. +00:30.486 | 55 | Config | 6 | 4 | begin 2, commit 2 | INSERT commit x1; DELETE commit x1; UPDATE commit x1; INSERT dynamicCommit x1; DELETE dynamicCommit x1; UPDATE dynamicCommit x1 | `commit` .. `dynamicCommit` |
| 33 | +00:30.490 .. +00:30.492 | 55 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 34 | +00:30.495 .. +00:30.501 | 55 | _ConfigChngR | 1 | 0 |  | DROP TABLE x1 |  |
| 35 | +00:30.502 .. +00:30.504 | 55 | _ConfigChngR_ExtProps | 1 | 0 |  | DROP TABLE x1 |  |
| 36 | +00:30.504 .. +00:30.505 | 55 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 37 | +00:30.508 .. +00:31.009 | 55 |  | 5 | 0 | begin 5, commit 5 | RENAME exec sp_rename '_ConfigChngRNG', '_ConfigChngR', 'object' x1; RENAME exec sp_rename '_ConfigChngR_ExtPropsNG', '_ConfigChngR_ExtProps', 'object' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_1NG', '_ConfigChngR_1', 'index' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_2NG', '_ConfigChngR_2', 'index' x1; RENAME exec sp_rename '_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG', '_ConfigChngR_ExtPro... x1 |  |
| 38 | +00:31.137 .. +00:31.141 | 55 | PK___ConfigC__AC8ED0C4E8279B32 | 1 | 0 |  | ALTER INDEX x1 |  |
| 39 | +00:31.164 .. +00:31.182 | 55 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 40 | +00:31.204 .. +00:31.222 | 55 | DBSchema | 1 | 1 |  | UPDATE x1 |  |
| 41 | +00:31.261 .. +00:31.292 | 55 | Params | 40 | 18 | begin 2, commit 2 | DELETE DBNames.New x1; DELETE DBNames-Ext-1.New x1; DELETE DBNames-Ext-<guid>.New x2; DELETE <guid>_dynupdate_<guid>.si <- Params x16; UPDATE <guid>.sinew -> <guid>_dynupdate_<guid>.si x16; DELETE siVersions x1; UPDATE siVersions x1; DELETE DynamicallyUpdated x1; ... +1 more | `DBNames.New` .. `DynamicallyUpdated` |
| 42 | +00:31.293 .. +00:31.295 | 55 | Config | 3 | 2 | begin 2, commit 1 | INSERT dbStruFinal x1; DELETE dbStruFinal x1; UPDATE dbStruFinal x1 | `dbStruFinal` |
| 43 | +00:31.363 .. +00:31.382 | 55 | _ConfigChngR | 1 | 49 |  | UPDATE x1 |  |
| 44 | +00:31.418 .. +00:31.500 | 55 | Config | 78 | 41 | begin 1, commit 1 | DELETE <guid>_dynupdate_<guid>.<n> <- Config x21; UPDATE <guid>.<n>.new -> <guid>_dynupdate_<guid>.<n> x21; DELETE <guid>_dynupdate_<guid> <- Config x14; UPDATE <guid>.new -> <guid>_dynupdate_<guid> x14; DELETE root <- Config x1; UPDATE root.new -> root x1; DELETE version <- Config x1; UPDATE version.new -> version x1; ... +4 more | `04332cbc-afdb-4c23-b188-b32fff0a1eb0_dynupdate_...` .. `DynamicallyUpdated` |
| 45 | +00:31.501 .. +00:31.505 | 55 | ConfigSave | 1 | 0 |  | DELETE LIKE % x1 | `LIKE %` |
| 46 | +00:31.509 .. +00:31.519 | 55 | Files | 2 | 2 |  | DELETE MobileVersions.dat <- Files x1; UPDATE MobileVersions.datNEW -> MobileVersions.dat x1 | `MobileVersions.dat` .. `MobileVersions.datNEW` |
| 47 | +00:31.520 .. +00:31.537 | 55 | Config | 3 | 0 |  | DELETE commit x1; DELETE dynamicCommit x1; DELETE dbStruFinal x1 | `commit` .. `dbStruFinal` |
| 48 | +00:31.567 .. +00:31.591 | 55 | Params | 4 | 2 | begin 2, commit 2 | DELETE <guid>.ui x2; UPDATE <guid>.ui x2 | `789702c6-d272-4008-9e55-db6f10687ae0.ui` .. `97f2c291-1aa3-4d96-8266-82a958c7dfaf.ui` |
| 49 | +00:31.923 .. +00:32.107 | 55 | Files | 30 | 18 | begin 3, commit 3 | DELETE userDocs_ru.new x2; INSERT userDocs_ru.new x1; UPDATE userDocs_ru.new x4; DELETE userVocabulary_ru.new x2; INSERT userVocabulary_ru.new x1; UPDATE userVocabulary_ru.new x4; DELETE userPostings_ru.new x2; INSERT userPostings_ru.new x1; ... +10 more | `userDocs_ru.new` .. `userPostings_ru.new` |
